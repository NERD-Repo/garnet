#!/boot/bin/sh

OUTPUT_DIR="${1}"

# Run Zircon microbenchmarks.
#
# Set the name of the results file to the unique ID to use in the
# performance dashboard and other post-processing systems.
zircon_benchmarks_results_file="${OUTPUT_DIR}/zircon.perftest"
/system/bin/zircon_benchmarks --fbenchmark_out="${zircon_benchmarks_results_file}"

# Run Ledger add_new_page tracing test.
add_new_page_results_file="${OUTPUT_DIR}/ledger.benchmarks.add_new_page"
trace record --spec-file=/system/ledger/benchmark/add_new_page.tspec \
             --benchmark-results-file=${add_new_page_results_file}
