#!/boot/bin/sh

RESULTS_FILE=$1

echo "Running tracing benchmark example"
trace record --spec-file=/system/data/benchmark_example/benchmark_example.tspec \
             --benchmark-results-file=${RESULTS_FILE}
