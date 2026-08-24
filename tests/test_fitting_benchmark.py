from scripts.benchmark_fitting import run_benchmark


def test_fitting_benchmark_small_matrix_is_complete():
    result = run_benchmark(rows=50, runs=1)
    assert result["rows"] == 50
    assert result["runs"] == 1
    assert set(result["methods"]) == {
        "polynomial",
        "exponential",
        "logarithmic",
        "power",
        "custom",
    }
    for metric in result["methods"].values():
        assert metric["median_elapsed_seconds"] >= 0
        assert metric["median_tracemalloc_peak_bytes"] >= 0
        assert metric["minimum_r2"] > 0.999999
