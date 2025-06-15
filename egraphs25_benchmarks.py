import sys
import os
import time
import shutil

from driver import *
import driver
driver.DEBUG = True

# Pool extraction disabled
# Strict 300 second timeout
global_params = {
    "mode": ("tracing-he-converge", ({"--ilp-iters": "2"})),
    "--egg-time-limit": "300",
    "--strict-deadlines": None
}

ESOP_PAPER_PATH = "esop_paper"
if shutil.which(ESOP_PAPER_PATH) is None:
    print("esop_paper is not in PATH!")
    exit(1)

def collect_traces(log_f, base, benchset, jobs):
    d = Driver.from_benchset_ruleset("all", benchset, None, f"{base}/eqsat_tracing")
    d.jobs = jobs
    d.capture_file = log_f

    baselines_dir = f"{base}/eqsat_tracing/baseline/"
    traces_dir = f"{base}/eqsat_tracing/trace/"
    if not os.path.exists(baselines_dir):
        os.mkdir(baselines_dir)
    if not os.path.exists(traces_dir):
        os.mkdir(traces_dir)
    
    inseqn_stem = lambda inseqn: pathlib.Path(inseqn).stem
    task_fn = lambda inseqn,a,_: d.run_wrap([ESOP_PAPER_PATH, inseqn,
        f"{baselines_dir}/{inseqn_stem(inseqn)}.eqn",
        f"{traces_dir}/{inseqn_stem(inseqn)}.trace"])
    d._run_all(task_fn)

def eval_dir(log_f, dir, jobs):
    benches=[]
    for bench in os.listdir(dir):
        benches.append(bench)
    units = []
    for bench in benches:
        units.append((f"{DRIVER_DIR}/bench/lobster/{bench}", None, f"{dir}/{bench}"))
    d = Driver(units, [])
    d.capture_file = log_f
    d.jobs = 1
    d.verify_all()
    d.jobs = jobs
    d.eval_all()

def opt(log_f, rules, base, benchset, jobs, trace_file_fn = None, opt_fn = None):
    rules_name = rules
    if rules is None:
        rules_name = "tracing"
    d = Driver.from_benchset_ruleset("all", benchset, rules, f"{base}/eqsat_{rules_name}")
    d.with_bool_rules()
    d.eqsatopt_params = global_params
    d.capture_file = log_f
    d.jobs = jobs
    d.trace_file_fn = trace_file_fn
    d.opt_all(opt_one_override=opt_fn)

def opt_one_traces_gen(base):
    if not os.path.exists(f"{base}/logs"):
        os.mkdir(f"{base}/logs")
    def opt_one_traces(driver, in_file, in_rules, out_file):
        in_file_stem = pathlib.Path(in_file).stem
        with open(f"{base}/logs/{in_file_stem}.log", 'w') as log_f:
            driver.opt_one(in_file, in_rules, out_file, capture_file_override=log_f)
    return opt_one_traces

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("expected a mode")
        exit(1)

    out_base = os.environ.get("OUT_BASE", "./")
    jobs_override = os.environ.get("JOBS_OVERRIDE", 0)

    mode = sys.argv[1]
    
    f = open(out_base + f"/{mode}.log", 'w')
    if mode == "eqsat_lobster_trace":
        jobs = 8 if jobs_override == 0 else jobs_override
        collect_traces(f, out_base, "lobster", jobs)
    elif mode == "eqsat_lobster_dac19_trace":
        jobs = 8 if jobs_override == 0 else jobs_override
        collect_traces(f, out_base, "lobster.opt_dac19", jobs)
    elif mode == "eqsat_lobster_opt":
        jobs = 4 if jobs_override == 0 else jobs_override
        trace_file_fn = lambda bench: f"{out_base}/eqsat_tracing/trace/{bench}.trace"
        opt(f, None, out_base, "lobster", jobs, trace_file_fn, opt_one_traces_gen(f"{out_base}/eqsat_tracing/"))
    elif mode == "eqsat_lobster_dac19_opt":
        jobs = 4 if jobs_override == 0 else jobs_override
        trace_file_fn = lambda bench: f"{out_base}/eqsat_tracing/trace/{bench}.trace"
        opt(f, None, out_base, "lobster.opt_dac19", jobs, trace_file_fn, opt_one_traces_gen(f"{out_base}/eqsat_tracing/"))
    elif mode == "eqsat_lobster_eval_base":
        jobs = 1 if jobs_override == 0 else jobs_override
        eval_dir(f, f"{out_base}/eqsat_tracing/baseline/", jobs)
    elif mode == "eqsat_lobster_eval_opt":
        jobs = 1 if jobs_override == 0 else jobs_override
        eval_dir(f, f"{out_base}/eqsat_lobster/opt/", jobs)
    else:
        print(f"unrecognized mode {mode}")
        exit(1)
    f.close()
