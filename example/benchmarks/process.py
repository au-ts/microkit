#!/usr/bin/env python3

"""
A simple script to take the outputs of these program (sum, sum²) and turn
them into the more useful mean and standard deviations.
"""

import argparse
import csv
import math
import sys

def process(results: csv.DictReader):
    writer = csv.DictWriter(sys.stdout, delimiter=",", fieldnames=["name", "mean", "stddev", "min", "max", "runs", "sum", "sum_squared"])
    writer.writeheader()

    for row in results:
        n = int(row["runs"], 16)
        sum_x = int(row["sum"], 16)
        sum_x_squared = int(row["sum_squared"], 16)
        mean = sum_x / n
        # sigma² = ( sum(x^2) - 2m*sum(x) + n*m^2 ) / n
        variance = ( sum_x_squared - 2 * mean * sum_x + n* (mean ** 2) ) / n
        stddev = math.sqrt(variance)

        writer.writerow({
            "name": row["name"],
            "mean": mean,
            "stddev": stddev,
            "min": int(row["min"], 16),
            "max": int(row["max"], 16),
            "runs": n,
            "sum": sum_x,
            "sum_squared": sum_x_squared,
        })

def strip_irrelevant(logstr: str) -> str:
    RESULTS_BEGIN = "__RESULTS_BEGIN__"
    RESULTS_END = "__RESULTS_END__"
    assert logstr.count(RESULTS_BEGIN) == 1
    assert logstr.count(RESULTS_END) == 1

    # for the begin we want the next line, so...
    index_begin = logstr.find(RESULTS_BEGIN) + len(RESULTS_BEGIN) + len("\n")
    index_end = logstr.find(RESULTS_END)
    return logstr[index_begin:index_end].strip()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("logfile")
    args = parser.parse_args()

    with open(args.logfile, newline="") as f:
        logstr = f.read()

    results_lines = strip_irrelevant(logstr).splitlines()
    reader = csv.DictReader(results_lines, delimiter=",")
    process(reader)
