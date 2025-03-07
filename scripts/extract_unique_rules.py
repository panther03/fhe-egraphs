import sys

def replace_operators(expr):
    return expr.replace("and", "*").replace("xor", "^").replace("not", "!")
def replace_identifies(expr):
    return expr.replace("i", "?i").replace("n", "?n")

with open(sys.argv[1]) as f:

    rules = set()
    old_bexp = None

    for line in f.readlines():

        expr = ""        
        if " : " in line and not "(or" in line:
            expr = line.split(" : ")[1]
        else:
            continue
        
        if line.startswith("old bexp"):
            old_bexp = replace_identifies(replace_operators(expr)).strip()
        elif line.startswith("new bexp"):
            assert old_bexp is not None
            rules.add(f"{old_bexp};{replace_identifies(replace_operators(expr)).strip()}")
    g = open(sys.argv[2], 'w') 
    g.write("\n".join(rules))
    g.close()