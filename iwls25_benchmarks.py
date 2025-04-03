import sys
import os
import time

from driver import *

# Pool extraction disabled
# Strict 300 second timeout
global_params = {
    "mode": ("md-multiple-iters", ({"--iters": "1", "--num-candidates": "0"})),
    "--egg-time-limit": "300",
    "--strict-deadlines": None
}

def eval_dir(log_f, dir, jobs):
    benches=[]
    for bench in os.listdir(dir):
        benches.append(bench)
    units = []
    for bench in benches:
        units.append((f"{DRIVER_DIR}/bench/esop_optimized/{bench}", None, f"{dir}/{bench}"))
    d = Driver(units, [])
    d.capture_file = log_f
    d.jobs = 1
    d.verify_all()
    d.jobs = jobs
    d.eval_all()


def opt_one_esop_wrap(log_f):
    ESOP_ITERS_LIMIT = 10
    def opt_one_esop(driver, in_file, in_rules, out_file):
        best_md = 0
        best_mc = 0
        iter = 0
        t0 = time.time()
        out_file_tmp = out_file + "_tmp.eqn"
        eq_ctr = 0
        while iter < ESOP_ITERS_LIMIT:
            if iter % 2 == 0:
                log_f.write(f"(bench {in_file}) Iter {iter}: eqsat (md={best_md},mc={best_mc})\n")
                log_f.flush()
                driver.opt_one(in_file, in_rules, out_file_tmp, None if iter == 0 else "60")
            else:
                log_f.write(f"(bench {in_file}) Iter {iter}: esop (md={best_md},mc={best_mc})\n")
                log_f.flush()
                r1 = subprocess.run([CKTCONV_PATH, "eqn2seqn", out_file, in_file], stdout=log_f, stderr=log_f)
                r2 = subprocess.run(["esop_apply", in_file, out_file_tmp], stdout=subprocess.DEVNULL, stderr=log_f)
                r3 = subprocess.run([CKTCONV_PATH, "eqn2seqn", out_file_tmp, in_file], stdout=log_f, stderr=log_f)
                if (r1.returncode | r2.returncode | r3.returncode):
                    log_f.write(f"(bench {in_file}) failed to run esop_apply\n")
                    log_f.flush()

            stats = subprocess.run([CKTCONV_PATH, "stats", out_file_tmp], capture_output=True)
            stats_split = stats.stdout.decode("utf-8").split(",")
            md = int(stats_split[0])
            mc = int(stats_split[1])

            if ((best_md != 0) and (best_mc != 0) and (md * md * mc > best_md * best_md * best_mc)):
                break
            else:
                if md == best_md and mc == best_mc:
                    eq_ctr += 1
                    if eq_ctr == 2:
                        break
                else:
                    eq_ctr = 0
                subprocess.run(["cp", out_file_tmp, out_file])
                best_md = md
                best_mc = mc
            iter += 1
        subprocess.run(["rm", "-f", out_file_tmp])
        t = time.time() - t0
        log_f.write(f"{in_file}:{t}\n")
        log_f.flush()
    return opt_one_esop


def opt(log_f, rules, base, jobs, opt_func = None):
    d = Driver.from_benchset_ruleset("all", "esop_optimized", rules, f"{base}/eqsat_{rules}")
    d.with_bool_rules()
    d.eqsatopt_params = global_params
    d.capture_file = log_f
    d.jobs = jobs
    d.opt_all(opt_func)

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("expected a mode")
        exit(1)

    out_base = os.environ.get("OUT_BASE", "./")
    jobs_override = os.environ.get("JOBS_OVERRIDE", 0)

    mode = sys.argv[1]
    
    f = open(out_base + f"/{mode}.log", 'w')
    if mode == "lobster_eval":
        jobs = 1 if jobs_override == 0 else jobs_override
        eval_dir(f, f"{DRIVER_DIR}/bench/lobster.opt_lobster", jobs)
    elif mode == "eqsat_lobster_opt":
        jobs = 8 if jobs_override == 0 else jobs_override
        opt(f, "lobster", out_base, jobs)
    elif mode == "eqsat_lobster_eval":
        jobs = 1 if jobs_override == 0 else jobs_override
        eval_dir(f, f"{out_base}/eqsat_lobster/opt/", jobs)
    elif mode == "eqsat_mcmd_opt":
        jobs = 4 if jobs_override == 0 else jobs_override
        opt(f, "mcmd", out_base, jobs, opt_func=opt_one_esop_wrap(f))
    elif mode == "eqsat_mcmd_eval":
        jobs = 1 if jobs_override == 0 else jobs_override
        eval_dir(f, f"{out_base}/eqsat_mcmd/opt/", jobs)
    elif mode == "mcmd_eval":
        jobs = 1 if jobs_override == 0 else jobs_override
        eval_dir(f, f"{DRIVER_DIR}/bench/lobster.opt_mcmd", jobs)
    else:
        print(f"unrecognized mode {mode}")
        exit(1)
    f.close()
