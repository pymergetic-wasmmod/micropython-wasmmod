"""CLI for `python -m rewrite.symmetry`."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .checker import SymmetryChecker, repo_root
from .report import print_human, write_markdown
from .scaffold import Scaffolder


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="rewrite.symmetry",
        description=(
            "MetalPython conversion tracker: *_rs file mirrors, pm_mpy_* API, "
            "SHA checkpoints, and progress history."
        ),
    )
    p.add_argument("--config", type=Path, default=None)
    p.add_argument("--tree", action="append", default=[], help="Limit to mirror name(s)")
    p.add_argument("--pm", action="store_true", help="Force pm_mpy_* section")
    p.add_argument("--pm-only", action="store_true")
    p.add_argument("--json", type=Path, default=None, help="Write full JSON report")
    p.add_argument("--markdown", type=Path, default=None, help="Write markdown summary")
    p.add_argument(
        "--next",
        type=int,
        default=0,
        dest="next_n",
        help="How many NEXT queue items to print (0 = all, default)",
    )
    p.add_argument(
        "--status",
        choices=["done", "gaps", "partial", "stub", "stale", "missing"],
        default=None,
        help="List all stems with this status",
    )
    p.add_argument(
        "--checkpoint",
        action="store_true",
        help="Write SHA checkpoint + append progress history (alias: --update-shas)",
    )
    p.add_argument(
        "--update-shas",
        action="store_true",
        help="Same as --checkpoint",
    )
    p.add_argument("--write-baseline", action="store_true")
    p.add_argument("--history", action="store_true", help="Show progress history and exit")
    p.add_argument(
        "--fail-on-ref-change",
        action="store_true",
        help="Exit 1 if upstream ref SHAs changed since checkpoint",
    )
    p.add_argument("--fail-on-regression", action="store_true")
    p.add_argument("--list-missing", action="store_true")
    p.add_argument("--list-modules", action="store_true")
    p.add_argument(
        "--scaffold-stubs",
        action="store_true",
        help="Create missing *_rs stub .rs files, mod/lib.rs trees, and Cargo workspace",
    )
    p.add_argument(
        "--scaffold-force",
        action="store_true",
        help="With --scaffold-stubs, overwrite existing stub .rs files",
    )
    return p


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    do_checkpoint = args.checkpoint or args.update_shas

    checker = SymmetryChecker(repo_root(), args.config)

    if args.history:
        print(checker.progress.format_history())
        return 0

    if args.list_modules:
        for mod in checker.pm.discover_modules():
            print(
                f"{mod['name']:<16} {mod['prefix']:<28} "
                f"{len(mod['exports']):3d} exports  {mod['source']}"
            )
        return 0

    if args.scaffold_stubs:
        scaffolder = Scaffolder(checker)
        pre = checker.scan(
            include_pm=False, compare_shas=False, compare_progress=False
        )
        if args.tree:
            want = set(args.tree)
            pre.mirrors = [m for m in pre.mirrors if m.name in want]
        result = scaffolder.run(
            pre, force=args.scaffold_force, write_cargo=True, write_mods=True
        )
        print(
            f"Scaffold: created {len(result.created)} stubs, "
            f"skipped {len(result.skipped)} existing, "
            f"wrote {len(result.mod_files)} mod/lib.rs, "
            f"{len(result.cargo_files)} Cargo.toml"
        )
        for p in result.created[:20]:
            print(f"  + {p}")
        if len(result.created) > 20:
            print(f"  … +{len(result.created) - 20} more")
        # Fall through to a fresh report after scaffolding.

    trees = set(args.tree) if args.tree else None
    # Full-repo SHA/history diffs only when scanning all mirrors (filtered
    # --tree would look like mass ref removals vs the checkpoint).
    full_scan = trees is None and not args.pm_only
    include_pm = (
        args.pm
        or args.pm_only
        or args.json
        or args.markdown
        or args.write_baseline
        or args.fail_on_regression
        or do_checkpoint
        or full_scan
    )
    report = checker.scan(
        trees=None if args.pm_only else trees,
        include_pm=include_pm,
        compare_shas=full_scan,
        compare_progress=full_scan,
    )
    if args.pm_only:
        report.mirrors = []

    show_pm = args.pm or args.pm_only or full_scan
    print_human(
        report,
        show_pm=show_pm,
        next_n=args.next_n,
        list_status=args.status,
    )

    prev_shas = checker.sha_store.state
    if (
        report.sha_diff is not None
        and not prev_shas
        and not args.pm_only
        and not do_checkpoint
        and not args.write_baseline
    ):
        print()
        print(
            "No SHA checkpoint yet. Create one with: "
            "python -m rewrite.symmetry --checkpoint"
        )

    if args.list_missing:
        print()
        print("Missing shadows:")
        for _, st in report.iter_stems("missing"):
            print(f"  {st.shadow}  <=  {', '.join(st.ref_files)}")

    if args.json:
        args.json.write_text(
            json.dumps(report.to_jsonable(), indent=2) + "\n", encoding="utf-8"
        )
        print(f"\nWrote JSON report to {args.json}")

    if args.markdown:
        write_markdown(report, args.markdown)
        print(f"Wrote markdown report to {args.markdown}")

    if args.write_baseline:
        checker.write_baseline(report)
        print(f"Wrote baseline to {checker.config.baseline_path}")

    if do_checkpoint or args.write_baseline:
        # Checkpoints always cover the full mirror set.
        if not full_scan:
            report = checker.scan(include_pm=True, compare_shas=True, compare_progress=True)
        result = checker.checkpoint(report)
        sha = result["sha"]
        print(
            f"Checkpoint: {checker.config.sha_path} "
            f"({len(sha['refs'])} refs, {len(sha['shadows'])} shadows); "
            f"history → {checker.config.history_path}"
        )

    if args.fail_on_ref_change and report.sha_diff is not None:
        d = report.sha_diff
        if d.refs_changed or d.refs_removed or d.refs_added:
            print("\nUpstream ref SHA changes detected.", file=sys.stderr)
            return 1

    if args.fail_on_regression:
        problems = checker.regressions(report)
        if problems:
            print("\nRegression failures:", file=sys.stderr)
            for prob in problems:
                print(f"  {prob}", file=sys.stderr)
            return 1
    return 0
