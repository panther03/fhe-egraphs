import numpy as np
import matplotlib as plt
import pandas as pd

#log3= pd.read_csv("log3.rb")
log3= pd.read_csv("log2")

datas = {}
for data in log3["a"]:
    if datas.get(data):
        datas[data] += 1
    else:
        datas[data] = 1

l = list(datas.items())
l.sort(key=lambda kv: kv[1], reverse=True)
print(l)


