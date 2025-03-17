import os
import pathlib
import subprocess
import sys
import argparse
import concurrent.futures

sys.tracebacklimit = 1

DRIVER_DIR = str(pathlib.Path(__file__).parent.resolve()) + "/"
EQSAT_OPT_PATH = DRIVER_DIR + "/eqsat-opt/target/release/eqsat-opt"
CKTCONV_PATH = DRIVER_DIR + "/ckt-convert/target/release/ckt-convert"
HE_EVAL_PATH = DRIVER_DIR + "/he-eval/build/he-eval"
RUN_ABC_PATH = DRIVER_DIR + "/scripts/run_abc.sh"

OUTDIR = "out"
OPTDIR = OUTDIR + "/opt"
DEBUG = False

def run_wrap(args):
    if DEBUG:
        print(args)
    return subprocess.run(args)

def parse_opts(args, opts_string):
    for opt in opts_string.split(" "):
        if "=" in opt:
            param, val = opt.split("=")
            args[param] = val
        else:
            args[opt] = None


class Driver:
    shared_rules = []
    units = []
    jobs = 1

    eqsatopt_params = {"mode": ("md-vanilla-flow",{})}

    def __init__(self, units, shared_rules):
        self.units = units
        self.shared_rules = shared_rules

    def with_arith_rules(self):
        self.shared_rules.append(f"{DRIVER_DIR}/rules/arith.rules")
        return self

    def with_bool_rules(self):
        self.shared_rules.append(f"{DRIVER_DIR}/rules/bool.rules")
        return self

    def with_esyn_rules(self):
        self.shared_rules.append("f{DRIVER_DIR}/rules/esyn.rules")
        return self

    @classmethod
    def from_benchset_ruleset(cls, benches, benchset, ruleset=None):
        if not os.path.exists(OUTDIR):
            os.mkdir(OUTDIR)
            if not os.path.exists(OPTDIR):
                os.mkdir(OPTDIR)
        benches_l = benches
        if benches == "all":
            benches_l = list(
                map(
                    lambda p: p.replace(".eqn", ""),
                    os.listdir(f"{DRIVER_DIR}/bench/{benchset}/"),
                )
            )
        for bench in benches_l:
            run_wrap([CKTCONV_PATH, "eqn2seqn", f"{DRIVER_DIR}/bench/{benchset}/{bench}.eqn", f"out/{bench}.seqn"])
        units = []
        for bench in benches_l:
            inseqn = f"out/{bench}.seqn"
            if ruleset:
                inrules = [f"{DRIVER_DIR}/rules/{ruleset}/{bench}.rules"]
            else: 
                inrules = []
            outeqn = f"out/opt/{bench}.eqn"
            units.append((inseqn, inrules, outeqn))
        driver = cls(units, [])
        return driver

    # bunch of methods that return a Driver based on things like the benchset and ruleset, etc.

    def _run_all(self, task):
        with concurrent.futures.ThreadPoolExecutor(max_workers=self.jobs) as executor:
            futures = []
            for unit in self.units:
                future = executor.submit(task, *unit)
                futures.append(future)
            concurrent.futures.wait(futures)
            for future in futures:
                if future.exception():
                    print(future.exception())

    def opt_all(self):
        if not self.eqsatopt_params.get("mode"):
            raise ValueError("Driver does not have a mode set!")
        # parallel executor thing
        def opt(in_file, in_rules, out_file):
            args = [EQSAT_OPT_PATH, in_file, out_file]
            for rule in self.shared_rules:
                args.append("--rules")
                args.append(rule)
            for rule in in_rules:
                args.append("--rules")
                args.append(rule)
            for (param,param_val) in self.eqsatopt_params.items():
                if param == "mode":
                    continue
                args.append(f"{param}")
                if param_val:
                    args.append(str(param_val))                
            (mode, mode_params) = self.eqsatopt_params["mode"]
            args.append(mode)
            for (mode_param,mode_param_val) in mode_params.items():
                args.append(f"{mode_param}")
                if mode_param_val:
                    args.append(str(mode_param_val))
            r = run_wrap(args)

        self._run_all(opt)

    def verify_all(self):
        def verify(in_file, _, out_file): 
            print(f"{in_file},", end="", flush=True)
            run_wrap([CKTCONV_PATH, "stats", out_file])
            print(",", end="", flush=True)
            run_wrap([RUN_ABC_PATH, in_file, out_file])
        self._run_all(verify)

    def eval_all(self):
        def eval(a,b, out_file):
            run_wrap([HE_EVAL_PATH, "-q", out_file])
        self._run_all(eval)


def cli_opt(args):
    if not args.bench and not args.all:
        raise RuntimeError("Must specify at least one bench or \"all\" option!")
    benches = args.bench
    if args.all:
        benches = "all"

    ruleset = None
    shared_rules = []
    if args.rules:
        if os.path.isdir(DRIVER_DIR + "/rules/" + args.rules):
            ruleset = args.rules
        else:
            if os.path.isfile(args.rules):
                shared_rules = [args.rules]
            else:
                assert os.path.isdir(args.rules)
                shared_rules = os.path.listdir(args.rules)
    d = Driver.from_benchset_ruleset(benches, args.benchset, ruleset)

    if args.domain == "int":
        d = d.with_arith_rules()
    elif args.domain == "bool":
        d = d.with_bool_rules()
    elif args.domain == "esyn":
        d = d.with_esyn_rules()

    d.shared_rules += shared_rules
    d.eqsatopt_params["mode"] = (args.mode,{})
    if args.modeopts:
        parse_opts(d.eqsatopt_params["mode"][1], args.modeopts)        
    
    if args.eqsatopts:
        parse_opts(d.eqsatopt_params, args.eqsatopts)

    if args.tl:
        d.eqsatopt_params["--egg-time-limit"] = args.tl
    
    

    d.jobs = args.j
    d.opt_all()

def cli_verify(args):
    references = []
    if os.path.isfile(args.reference):
        references.append(args.reference)
    else:
        assert os.path.isdir(args.reference)
        for reference in os.listdir(args.reference):
            references.append(args.reference + '/' + reference)
    opted = []
    if os.path.isfile(args.opted):
        opted.append(args.opted)
    else:
        assert os.path.isdir(args.opted)
        for opted_f in os.listdir(args.opted):
            opted.append(args.opted + '/' + opted_f)
    assert len(opted) == len(references)
    units = []
    for i in range(len(references)):
        units.append((references[i],None,opted[i]))
    d = Driver(units, [])
    d.verify_all()

def cli_eval(args):
    units = []
    if os.path.isfile(args.eqns):
        units.append((None, None, args.eqns))
    else:
        assert os.path.isdir(args.eqns)
        for eqn in os.listdir(args.eqns):
            units.append((None, None, args.eqns + '/' + eqn))
    d = Driver(units, [])
    d.jobs = args.j
    d.eval_all()

if __name__ == "__main__":
    parser = argparse.ArgumentParser(prog="driver")
    parser.add_argument("-j", type=int, default=1, metavar="N")
    parser.add_argument("--debug", "-d", action='store_true')
    subparsers = parser.add_subparsers(required=True)
    
    opt_parser = subparsers.add_parser("opt")
    opt_parser.add_argument("benchset", type=str)
    opt_parser.add_argument("--rules", type=str, default=None)
    opt_parser.add_argument("--all", action='store_true')
    opt_parser.add_argument("--bench", type=str, action='append')
    opt_parser.add_argument("--mode", type=str, default="md-multiple-iters")
    opt_parser.add_argument("--modeopts", type=str)
    opt_parser.add_argument("--eqsatopts", type=str)
    opt_parser.add_argument("--tl", type=int, help="Egg time limit", default=None)
    opt_parser.add_argument("--domain", choices=["int", "esyn", "bool", "none"], help="Domain (selects what shared rules to use)", default="bool")
    opt_parser.set_defaults(func=cli_opt)

    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("reference", type=str)
    verify_parser.add_argument("opted", type=str, default="out/opt")
    verify_parser.set_defaults(func=cli_verify)

    eval_parser = subparsers.add_parser("eval")
    eval_parser.add_argument("eqns", type=str, default="out/opt")
    eval_parser.set_defaults(func=cli_eval)

    args = parser.parse_args()
    if args.debug:
        DEBUG = True
    args.func(args)
    
    