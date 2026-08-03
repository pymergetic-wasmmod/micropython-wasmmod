"""Human / markdown report rendering."""

from __future__ import annotations

from pathlib import Path

from .models import FullReport, ShaDiff


def format_counts_line(name: str, counts: dict[str, int], pct: float, width: int = 14) -> str:
    return (
        f"{name:<{width}}"
        f"{counts.get('done', 0):4d} done  "
        f"{counts.get('gaps', 0):3d} gaps  "
        f"{counts.get('partial', 0):3d} partial  "
        f"{counts.get('stub', 0):3d} stub  "
        f"{counts.get('stale', 0):3d} stale  "
        f"{counts.get('missing', 0):4d} missing  "
        f"({pct:5.1f}%)"
    )


def _print_change_list(title: str, items: list[str], limit: int = 12) -> None:
    if not items:
        return
    print(f"  {title}: {len(items)}")
    for p in items[:limit]:
        print(f"    {p}")
    if len(items) > limit:
        print(f"    … +{len(items) - limit} more")


def print_sha_diff(diff: ShaDiff) -> None:
    print()
    if diff.checkpoint_at:
        print(f"Since SHA checkpoint {diff.checkpoint_at}")
    else:
        print("Since SHA checkpoint (none yet)")
    print("-" * 72)
    if not diff.any:
        print("  (no SHA changes)")
        return
    _print_change_list("refs added", diff.refs_added)
    _print_change_list("refs changed", diff.refs_changed)
    _print_change_list("refs removed", diff.refs_removed)
    _print_change_list("shadows added", diff.shadows_added)
    _print_change_list("shadows changed", diff.shadows_changed)
    _print_change_list("shadows removed", diff.shadows_removed)
    _print_change_list("stems now stale (upstream moved)", diff.stale_stems)


def _fmt_conv_row(label: str, row: dict, width: int = 12) -> str:
    return (
        f"  {label:<{width}}"
        f"{row.get('done', 0):4d} done  "
        f"{row.get('gaps', 0):3d} gaps  "
        f"{row.get('partial', 0):3d} partial  "
        f"{row.get('stub', 0):3d} stub  "
        f"{row.get('stale', 0):3d} stale  "
        f"{row.get('missing', 0):4d} missing  "
        f"(n={row.get('total', 0)})"
    )


def print_conversion_stats(report: FullReport) -> None:
    """Show .c/.h/.py/… → .rs conversion breakdown."""
    stats = report.conversion_stats()
    if not stats["stems_total"]:
        return

    print()
    print(
        f"Conversion  {stats['ref_files_total']} source files → "
        f"{stats['stems_total']} shadow stems"
    )
    print("-" * 72)

    print("  by source file type (each ref file counted once):")
    for row in stats["by_source_ext"]:
        src = row["key"]
        print(_fmt_conv_row(f"{src} → .rs", row, width=14))

    print("  by stem shape (merged inputs → one shadow):")
    for row in stats["by_stem_shape"]:
        parts = row["key"].split("+")
        label = "+".join(f".{p}" for p in parts) + " → .rs"
        print(_fmt_conv_row(label, row, width=18))

    print("  by shadow target:")
    for row in stats["by_shadow_ext"]:
        print(_fmt_conv_row(row["key"], row, width=14))

    # Compact converted vs remaining
    doneish = tot_done = 0
    missingish = 0
    for row in stats["by_stem_shape"]:
        tot_done += row["done"] + row.get("gaps", 0) + row["partial"] + row["stub"]
        missingish += row["missing"] + row["stale"]
        doneish += row["done"]
    print(
        f"  summary  {doneish} done stems, "
        f"{tot_done - doneish} in-progress (gaps/partial/stub), "
        f"{missingish} remaining (missing/stale)"
    )


def print_progress_delta(delta: dict) -> None:
    print()
    print(f"Progress since {delta.get('since', '?')}")
    print("-" * 72)
    fp = delta.get("file_progress_pct", {})
    pp = delta.get("pm_progress_pct", {})
    print(f"  files: {fp.get('from', 0)}% → {fp.get('to', 0)}%  ({fp.get('delta', 0):+g}%)")
    print(f"  pm:    {pp.get('from', 0)}% → {pp.get('to', 0)}%  ({pp.get('delta', 0):+g}%)")
    _print_change_list("stems newly done", delta.get("stems_newly_done", []))
    _print_change_list("stems no longer done", delta.get("stems_no_longer_done", []))
    _print_change_list("symbols newly present", delta.get("symbols_newly_present", []))
    _print_change_list("symbols no longer present", delta.get("symbols_no_longer_present", []))


def print_human(
    report: FullReport,
    *,
    show_pm: bool,
    next_n: int,
    list_status: str | None = None,
) -> None:
    print("MetalPython rewrite symmetry")
    print("=" * 72)
    if report.mirrors and (report.pm_modules or report.pm_infra):
        print(
            f"Overall  files {report.file_progress_pct():5.1f}%   "
            f"pm_mpy {report.pm_progress_pct():5.1f}%"
        )
    elif report.mirrors:
        print(f"Overall  files {report.file_progress_pct():5.1f}%")
    elif report.pm_modules or report.pm_infra:
        print(f"Overall  pm_mpy {report.pm_progress_pct():5.1f}%")
    print("-" * 72)

    for m in report.mirrors:
        c = m.counts()
        tracked = sum(c[s] for s in ("done", "gaps", "partial", "stub", "stale", "missing"))
        if tracked == 0 and not m.stems:
            print(f"{m.name + '/':<14}(disabled or empty)")
            continue
        label = m.ref if m.ref.endswith("/") else m.ref + "/"
        print(format_counts_line(label, c, m.progress_pct()))

    tot = report.total_counts()
    tracked = sum(tot[s] for s in ("done", "gaps", "partial", "stub", "stale", "missing"))
    if tracked:
        print("-" * 72)
        print(format_counts_line("TOTAL", tot, report.file_progress_pct()))

    if report.mirrors:
        print_conversion_stats(report)

    if report.sha_diff is not None:
        print_sha_diff(report.sha_diff)
    if report.progress_delta is not None:
        print_progress_delta(report.progress_delta)

    if show_pm:
        print()
        print("pm_mpy_* API (discovered from MP_REGISTER_MODULE*)")
        print("-" * 72)
        for name, syms in report.pm_modules.items():
            present = sum(1 for s in syms if s.status == "present")
            partial = sum(1 for s in syms if s.status == "partial")
            stub = sum(1 for s in syms if s.status == "stub")
            missing = sum(1 for s in syms if s.status == "missing")
            total = len(syms)
            print(
                f"  {name:<16} {present:3d} present  {partial:3d} partial  "
                f"{stub:3d} stub  {missing:3d} missing  ({present}/{total})"
            )
        if report.pm_infra:
            c = report.pm_counts()
            # infra alone
            present = sum(1 for s in report.pm_infra if s.status == "present")
            stub = sum(1 for s in report.pm_infra if s.status == "stub")
            missing = sum(1 for s in report.pm_infra if s.status == "missing")
            total = len(report.pm_infra)
            print(
                f"  {'infra':<16} {present:3d} present  {0:3d} partial  "
                f"{stub:3d} stub  {missing:3d} missing  ({present}/{total})"
            )
        pc = report.pm_counts()
        print(
            f"  {'TOTAL':<16} {pc.get('present', 0):3d} present  "
            f"{pc.get('partial', 0):3d} partial  {pc.get('stub', 0):3d} stub  "
            f"{pc.get('missing', 0):3d} missing  "
            f"({pc.get('present', 0)}/{pc.get('total', 0)})"
        )

    if list_status:
        print()
        print(f"Stems with status={list_status}:")
        n = 0
        for m, st in report.iter_stems(list_status):
            print(f"  {st.shadow}  <=  {', '.join(st.ref_files)}")
            n += 1
        if n == 0:
            print("  (none)")

    # Suggested file-conversion queue (not pm_mpy_* — that has its own section).
    # Priority: stale → missing → stub → partial. Full list unless --next N.
    next_items: list[tuple[str, str]] = []  # (status, shadow_path)
    for want in ("stale", "missing", "stub", "partial"):
        for _, st in report.iter_stems(want):
            next_items.append((want, st.shadow))

    if next_items:
        # next_n == 0 → show everything; otherwise preview first N.
        limit = None if next_n == 0 else max(0, next_n)
        shown = next_items if limit is None else next_items[:limit]
        print()
        if limit is None or len(next_items) <= len(shown):
            print(f"NEXT work queue ({len(next_items)} shadow stems):")
        else:
            print(
                f"NEXT work queue (showing {len(shown)} of {len(next_items)}; "
                f"pass --next 0 for the full list):"
            )
        for kind, item in shown:
            print(f"  [{kind}] {item}")
        if limit is not None and len(next_items) > limit:
            print(f"  … {len(next_items) - limit} more omitted")


def write_markdown(report: FullReport, path: Path) -> None:
    lines = [
        "# MetalPython rewrite symmetry",
        "",
        f"- **Files:** {report.file_progress_pct():.1f}%",
        f"- **pm_mpy_*:** {report.pm_progress_pct():.1f}%",
        "",
        "## Trees",
        "",
        "| Tree | Done | Partial | Stub | Stale | Missing | Progress |",
        "|------|-----:|--------:|-----:|------:|--------:|---------:|",
    ]
    for m in report.mirrors:
        c = m.counts()
        if not m.stems and sum(c.values()) == 0:
            continue
        lines.append(
            f"| `{m.ref}/` | {c.get('done', 0)} | {c.get('partial', 0)} | "
            f"{c.get('stub', 0)} | {c.get('stale', 0)} | {c.get('missing', 0)} | "
            f"{m.progress_pct():.1f}% |"
        )
    tot = report.total_counts()
    lines.append(
        f"| **TOTAL** | {tot.get('done', 0)} | {tot.get('partial', 0)} | "
        f"{tot.get('stub', 0)} | {tot.get('stale', 0)} | {tot.get('missing', 0)} | "
        f"{report.file_progress_pct():.1f}% |"
    )

    stats = report.conversion_stats()
    if stats["stems_total"]:
        lines += [
            "",
            "## Conversion (.c/.h/.py → .rs)",
            "",
            f"{stats['ref_files_total']} source files → {stats['stems_total']} shadow stems",
            "",
            "### By source file type",
            "",
            "| Source | Done | Partial | Stub | Stale | Missing | Total |",
            "|--------|-----:|--------:|-----:|------:|--------:|------:|",
        ]
        for row in stats["by_source_ext"]:
            lines.append(
                f"| `{row['key']} → .rs` | {row['done']} | {row['partial']} | "
                f"{row['stub']} | {row['stale']} | {row['missing']} | {row['total']} |"
            )
        lines += [
            "",
            "### By stem shape",
            "",
            "| Shape | Done | Partial | Stub | Stale | Missing | Total |",
            "|-------|-----:|--------:|-----:|------:|--------:|------:|",
        ]
        for row in stats["by_stem_shape"]:
            parts = row["key"].split("+")
            label = "+".join(f".{p}" for p in parts) + " → .rs"
            lines.append(
                f"| `{label}` | {row['done']} | {row['partial']} | "
                f"{row['stub']} | {row['stale']} | {row['missing']} | {row['total']} |"
            )

    lines += ["", "## pm_mpy_* modules", ""]
    lines.append("| Module | Present | Missing | Total |")
    lines.append("|--------|--------:|--------:|------:|")
    for name, syms in report.pm_modules.items():
        present = sum(1 for s in syms if s.status == "present")
        missing = sum(1 for s in syms if s.status == "missing")
        lines.append(f"| `{name}` | {present} | {missing} | {len(syms)} |")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
