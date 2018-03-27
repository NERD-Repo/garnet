#!/boot/bin/sh

RESULTS_FILE=$1

/system/bin/zircon_benchmarks --fbenchmark_out="${RESULTS_FILE}"
