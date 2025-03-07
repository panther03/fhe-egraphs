import os

for case_file in os.listdir("default"):
    with open("default/" + case_file) as f:
        opt_targets = []
        input_lists = []
        old_bexps = []
        new_bexps = []
        for line in f.readlines():
            if line.startswith("opt target"):
                opt_targets.append(line)
            elif line.startswith("Input list"):
                input_lists.append(line)
            elif line.startswith("old bexp"):
                old_bexps.append(line)
            elif line.startswith("new bexp"):
                new_bexps.append(line)
        no_ors = filter(lambda t: " or " not in t[2] and " or " not in t[3], zip(opt_targets,input_lists,old_bexps,new_bexps))
        with open("no_or/" + case_file, 'w') as g:
            for (a,b,c,d) in no_ors:
                g.write(a)
                g.write(b)
                g.write(c)
                g.write(d)
                g.write("\n\n")