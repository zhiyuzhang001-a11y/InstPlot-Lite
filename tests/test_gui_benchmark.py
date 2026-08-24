from scripts.benchmark_gui import run_benchmark


def test_gui_benchmark_small_matrix_is_structured_and_complete():
    result = run_benchmark(rows=100, runs=1, startup_runs=1)

    assert result["rows"] == 100
    assert result["runs"] == 1
    assert result["startup_runs"] == 1
    for name in [
        "cold_startup",
        "text_import",
        "processing",
        "polynomial_fit",
        "rectangle_scan",
        "pan_event",
        "full_redraw",
    ]:
        metric = result["metrics"][name]
        assert len(metric["elapsed_seconds"]) == 1
        assert metric["median_elapsed_seconds"] >= 0
        assert metric["median_tracemalloc_peak_bytes"] >= 0
