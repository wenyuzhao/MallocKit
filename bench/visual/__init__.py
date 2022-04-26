import subprocess
from typing import Tuple
from IPython.display import Markdown, display
import pandas as pd
import platform
import psutil
import math
from pandas.api.types import is_numeric_dtype
import numpy as np


class Pipeline:
    @staticmethod
    def load_results() -> pd.DataFrame:
        return pd.read_csv('./_logs/results.csv')

    @staticmethod
    def mean_over_invocation(df: pd.DataFrame) -> Tuple[pd.DataFrame, int]:
        df = df.copy()
        benchmarks = set(df['bench'])
        mallocs = set(df['malloc'])
        # metrics = [c for c in df.columns if c not in ['bench', 'malloc', 'invocation']]
        data = {}
        invocations = 0
        for index, row in df.iterrows():
            key = (row['malloc'], row['bench'])
            if key not in data:
                data[key] = []
            data[key].append(index)
        drop_indexes = []
        for bm in benchmarks:
            for a in mallocs:
                idx = data[(a, bm)]
                invocations = len(idx)
                avg = df.loc[idx].mean(axis=0, numeric_only=True, skipna=True)
                v = [v for i, v in avg.items()]
                i = [i for i, v in avg.items()]
                df.loc[idx[0], i] = v
                drop_indexes += idx[1:]
        df.drop(drop_indexes, inplace=True)
        df.drop(['invocation'], axis=1, inplace=True)
        df.reset_index(drop=True, inplace=True)
        return (df, invocations)

    @staticmethod
    def filter(df: pd.DataFrame, select: pd.DataFrame) -> pd.DataFrame:
        return df[select]

    @staticmethod
    def normalize(df: pd.DataFrame, baseline: str) -> pd.DataFrame:
        def apply(x):
            y = x.copy()
            for col in x.columns.values:
                if is_numeric_dtype(y[col]):
                    y[col] = y[col] / x.loc[x['malloc'] == baseline][col].iloc[0]
            return y
        return df.groupby(['bench']).apply(apply)

    @staticmethod
    def plot_bar(df: pd.DataFrame, series: str, pivot: str, value: str) -> pd.DataFrame:
        pivot = pd.pivot_table(df, values=value, index=pivot, columns=series)
        # Calculate min/max/mean/geomean
        min = pivot.apply(lambda x: np.min(x), axis=0)
        max = pivot.apply(lambda x: np.max(x), axis=0)
        mean = pivot.apply(lambda x: np.mean(x), axis=0)
        geomean = pivot.apply(lambda x: np.exp(np.mean(np.log(x))), axis=0)
        pivot.loc['.'] = pivot.apply(lambda x: 0, axis=0)
        pivot.loc['min'] = min
        pivot.loc['max'] = max
        pivot.loc['mean'] = mean
        pivot.loc['geomean'] = geomean
        # Plot
        pivot.plot(kind="bar", figsize=(20, 5), rot=45)
        # Format table for output
        pivot.loc['.'] = pivot.apply(lambda x: '', axis=0)
        pivot.loc['min'] = min
        pivot.loc['max'] = max
        pivot.loc['mean'] = mean
        pivot.loc['geomean'] = geomean
        return pivot


def markdown(s: str):
    display(Markdown(s))


def display_meta_info():
    git_revision = subprocess.check_output(
        ['git', 'rev-parse', '--short', 'HEAD'], text=True).strip()
    uname = platform.uname()
    os = f'{uname.system} ({uname.release})'
    cpu = subprocess.check_output(
        "lscpu | grep 'Model name:' | sed -r 's/Model name:\s{1,}//g'", shell=True, text=True).strip()
    mem = str(math.ceil(psutil.virtual_memory().total / (1024.0 ** 3))) + ' GB'
    markdown(
        f'| Meta          | Value          |\n'
        f'|:------------- | --------------:|\n'
        f'| Git revision  | {git_revision} |\n'
        f'| System        | {os}           |\n'
        f'| Processor     | {cpu}          |\n'
        f'| Memory        | {mem}          |\n'
    )
