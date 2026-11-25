"""Statistics and schedule order for paired Merman benchmark evidence."""

from __future__ import annotations

import hashlib
import math
import random
import statistics
from dataclasses import asdict, dataclass
from typing import Any, Sequence


DECISION_GRADE_BOOTSTRAP_RESAMPLES = 10_000
MAX_BOOTSTRAP_RESAMPLES = 100_000


@dataclass(frozen=True)
class PairCount:
    required_pairs: int
    scheduled_pairs: int
    exceeds_cap: bool


def stable_seed(seed: int, *parts: object) -> int:
    material = "\x1f".join((str(seed), *(str(part) for part in parts)))
    return int.from_bytes(hashlib.sha256(material.encode("utf-8")).digest()[:8], "big")


def pair_order(pair_index: int, *, start_with_a: bool) -> str:
    """Alternate AB and BA from the declared starting arm."""
    return "AB" if (pair_index % 2 == 0) == start_with_a else "BA"


def _percentile_from_sorted(ordered: Sequence[float], probability: float) -> float:
    if not ordered:
        raise ValueError("percentile requires at least one value")
    if len(ordered) == 1:
        return ordered[0]
    position = max(0.0, min(1.0, probability)) * (len(ordered) - 1)
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    fraction = position - lower
    return ordered[lower] * (1.0 - fraction) + ordered[upper] * fraction


def _bootstrap_mean_bounds(
    values: Sequence[float],
    *,
    seed: int,
    resamples: int,
    confidence_level: float,
    interval: str = "one_sided",
) -> dict[str, float]:
    if not values:
        raise ValueError("paired bounds require at least one pair")
    if not 0.0 < confidence_level < 1.0:
        raise ValueError("confidence_level must be between zero and one")
    if resamples <= 0:
        raise ValueError("resamples must be positive")
    if resamples > MAX_BOOTSTRAP_RESAMPLES:
        raise ValueError(
            f"resamples must be at most {MAX_BOOTSTRAP_RESAMPLES}"
        )
    estimate = statistics.fmean(values)
    if len(values) == 1 or all(value == values[0] for value in values):
        return {"estimate": estimate, "lower": estimate, "upper": estimate}
    rng = random.Random(seed)
    n = len(values)
    samples = sorted(
        statistics.fmean(values[rng.randrange(n)] for _ in range(n))
        for _ in range(resamples)
    )
    alpha = 1.0 - confidence_level
    if interval == "one_sided":
        lower_probability = alpha
        upper_probability = confidence_level
    elif interval == "two_sided":
        lower_probability = alpha / 2.0
        upper_probability = 1.0 - alpha / 2.0
    else:
        raise ValueError(f"unknown interval kind: {interval}")
    return {
        "estimate": estimate,
        "lower": _percentile_from_sorted(samples, lower_probability),
        "upper": _percentile_from_sorted(samples, upper_probability),
    }


def _mirror_bounds(bounds: dict[str, float]) -> dict[str, float]:
    return {
        "estimate": -bounds["estimate"],
        "lower": -bounds["upper"],
        "upper": -bounds["lower"],
    }


def paired_bounds(
    *,
    base_ns: Sequence[float],
    head_ns: Sequence[float],
    confidence_level: float = 0.95,
    bootstrap_seed: int = 0,
    bootstrap_resamples: int = DECISION_GRADE_BOOTSTRAP_RESAMPLES,
    interval: str = "one_sided",
    family_size: int = 2,
) -> dict[str, Any]:
    """Return deterministic paired bounds in the canonical head/base direction."""
    if len(base_ns) != len(head_ns) or not base_ns:
        raise ValueError("base/head samples must contain the same non-zero pair count")
    if any(not math.isfinite(value) or value <= 0.0 for value in (*base_ns, *head_ns)):
        raise ValueError("paired samples must be finite and positive")
    if isinstance(family_size, bool) or not isinstance(family_size, int) or family_size <= 0:
        raise ValueError("family_size must be a positive integer")
    component_confidence_level = 1.0 - (1.0 - confidence_level) / family_size
    log_ratios = [math.log(head / base) for base, head in zip(base_ns, head_ns)]
    absolute = [head - base for base, head in zip(base_ns, head_ns)]
    log_bounds = _bootstrap_mean_bounds(
        log_ratios,
        seed=bootstrap_seed,
        resamples=bootstrap_resamples,
        confidence_level=component_confidence_level,
        interval=interval,
    )
    absolute_bounds = _bootstrap_mean_bounds(
        absolute,
        seed=bootstrap_seed ^ 0x5F3759DF,
        resamples=bootstrap_resamples,
        confidence_level=component_confidence_level,
        interval=interval,
    )
    return {
        "log_ratio": log_bounds,
        "absolute_ns": absolute_bounds,
        "improvement_log_ratio": _mirror_bounds(log_bounds),
        "improvement_absolute_ns": _mirror_bounds(absolute_bounds),
        "confidence_contract": {
            "simultaneous_confidence_level": confidence_level,
            "component_confidence_level": component_confidence_level,
            "family_size": family_size,
            "multiplicity_adjustment": "bonferroni",
        },
    }


def classify_confirmation(
    bounds: dict[str, dict[str, float]],
    *,
    relative_threshold: float,
    absolute_threshold_ns: float,
    direction: str,
    evidence_mode: str,
    pair_count: int,
    required_pairs: int,
) -> str:
    if evidence_mode == "diagnostic":
        if not 1 <= pair_count <= 4:
            raise ValueError("diagnostic evidence requires one to four pairs")
        return "diagnostic_advisory"
    if evidence_mode != "confirmation":
        raise ValueError(f"unknown evidence mode: {evidence_mode}")
    if pair_count < 8 or pair_count < required_pairs:
        return "inconclusive"

    relative = bounds["log_ratio"]
    absolute = bounds["absolute_ns"]
    if direction == "regression":
        if relative["lower"] > relative_threshold and absolute["lower"] > absolute_threshold_ns:
            return "confirmed_regression"
        if relative["upper"] <= relative_threshold or absolute["upper"] <= absolute_threshold_ns:
            return "confirmed_non_regression"
        return "inconclusive"
    if direction == "improvement":
        if relative["upper"] < -relative_threshold and absolute["upper"] < -absolute_threshold_ns:
            return "confirmed_improvement"
        if relative["lower"] >= -relative_threshold or absolute["lower"] >= -absolute_threshold_ns:
            return "confirmed_non_improvement"
        return "inconclusive"
    raise ValueError(f"unknown confirmation direction: {direction}")


def required_pair_count(
    *, sigma: float, minimum_detectable_effect: float, max_pairs: int
) -> PairCount:
    if not math.isfinite(sigma) or sigma < 0.0:
        raise ValueError("sigma must be finite and non-negative")
    if not math.isfinite(minimum_detectable_effect) or minimum_detectable_effect <= 0.0:
        raise ValueError("minimum_detectable_effect must be finite and positive")
    if max_pairs < 8:
        raise ValueError("max_pairs must be at least eight")
    raw = max(8, math.ceil(((1.645 + 0.842) * sigma / minimum_detectable_effect) ** 2))
    required = raw if raw % 2 == 0 else raw + 1
    return PairCount(
        required_pairs=required,
        scheduled_pairs=min(required, max_pairs),
        exceeds_cap=required > max_pairs,
    )


def joint_calibration_policy(
    side_summaries: dict[str, dict[str, Any]], *, max_pairs: int
) -> dict[str, Any]:
    """Freeze one certificate using only A/A, without changing generic calibration."""
    metrics: dict[str, Any] = {}
    for metric, key, count_key in (
        ("log_ratio", "log", "relative"),
        ("absolute_ns", "absolute", "absolute"),
    ):
        reasons: list[str] = []
        counts = []
        for side in ("base", "head"):
            summary = side_summaries[side]
            if summary.get("status") == "contract_failure" or summary.get("complete_pairs") != summary.get("expected_pairs"):
                reasons.append(f"{side}_incomplete_calibration")
            for kind in ("identity", "order"):
                if summary.get("checks", {}).get(f"{kind}_{key}_within_margin") is not True:
                    reasons.append(f"{side}_{kind}_{key}_within_margin")
            count = summary.get("required_pairs", {}).get(count_key, {}).get("required_pairs")
            if type(count) is not int or count < 8 or count % 2:
                reasons.append(f"{side}_invalid_required_pairs")
            else:
                counts.append(count)
        required = max(counts) if len(counts) == 2 else None
        if required is not None and required > max_pairs:
            reasons.append("required_pairs_exceed_cap")
        metrics[metric] = {"qualified": not reasons, "required_pairs": required, "reasons": reasons}
    eligible = [metric for metric in metrics if metrics[metric]["qualified"]]
    selected = min(eligible, key=lambda metric: metrics[metric]["required_pairs"]) if eligible else None
    return {
        "selected_metric": selected,
        "eligible": selected is not None,
        "metrics": metrics,
        "required_pairs": metrics[selected]["required_pairs"] if selected else None,
    }


def classify_joint_confirmation(
    bounds: dict[str, Any], *, policy: dict[str, Any], pair_count: int,
    fixed_pairs: int, relative_threshold: float, absolute_threshold_ns: float,
) -> str:
    if pair_count != fixed_pairs or pair_count < 8 or pair_count % 2:
        return "inconclusive"
    selected = policy["selected_metric"]
    thresholds = {"log_ratio": relative_threshold, "absolute_ns": absolute_threshold_ns}
    if selected is None:
        return "inconclusive"
    certificate = policy["metrics"][selected]
    if not certificate["qualified"] or pair_count < certificate["required_pairs"]:
        return "inconclusive"
    if bounds[selected]["upper"] <= thresholds[selected]:
        return "confirmed_non_regression"
    if all(
        metric["qualified"] and pair_count >= metric["required_pairs"]
        and bounds[name]["lower"] > thresholds[name]
        for name, metric in policy["metrics"].items()
    ):
        return "confirmed_regression"
    return "inconclusive"


def bounds_with_relative_percent(bounds: dict[str, Any]) -> dict[str, Any]:
    relative = {
        key: math.expm1(value) * 100.0
        for key, value in bounds["log_ratio"].items()
    }
    improvement_relative = {
        key: math.expm1(value) * 100.0
        for key, value in bounds["improvement_log_ratio"].items()
    }
    return {
        **bounds,
        "relative_percent": relative,
        "improvement_relative_percent": improvement_relative,
    }


def _within_equivalence_margin(bounds: dict[str, float], margin: float) -> bool:
    return bounds["lower"] >= -margin and bounds["upper"] <= margin


def summarize_calibration(
    pairs: Sequence[dict[str, Any]],
    *,
    expected_pairs: int,
    relative_mde: float,
    absolute_mde_ns: float,
    max_pairs: int,
    confidence_level: float,
    bootstrap_seed: int,
    bootstrap_resamples: int,
    confidence_family_size: int = 2,
) -> dict[str, Any]:
    if len(pairs) != expected_pairs:
        return {
            "status": "contract_failure",
            "stable": False,
            "complete_pairs": len(pairs),
            "expected_pairs": expected_pairs,
            "reason": "A/A schedule did not produce every preregistered pair",
        }
    role_a = [pair["a"]["normalized_ns"] for pair in pairs]
    role_b = [pair["b"]["normalized_ns"] for pair in pairs]
    first = [pair["first"]["normalized_ns"] for pair in pairs]
    second = [pair["second"]["normalized_ns"] for pair in pairs]
    identity = paired_bounds(
        base_ns=role_a,
        head_ns=role_b,
        confidence_level=confidence_level,
        bootstrap_seed=bootstrap_seed,
        bootstrap_resamples=bootstrap_resamples,
        interval="two_sided",
        family_size=confidence_family_size,
    )
    order_effect = paired_bounds(
        base_ns=first,
        head_ns=second,
        confidence_level=confidence_level,
        bootstrap_seed=bootstrap_seed ^ 0xA5A5A5A5,
        bootstrap_resamples=bootstrap_resamples,
        interval="two_sided",
        family_size=confidence_family_size,
    )
    log_values = [math.log(value_b / value_a) for value_a, value_b in zip(role_a, role_b)]
    absolute_values = [value_b - value_a for value_a, value_b in zip(role_a, role_b)]
    sigma_log = statistics.stdev(log_values)
    sigma_absolute = statistics.stdev(absolute_values)
    relative_count = required_pair_count(
        sigma=sigma_log,
        minimum_detectable_effect=relative_mde,
        max_pairs=max_pairs,
    )
    absolute_count = required_pair_count(
        sigma=sigma_absolute,
        minimum_detectable_effect=absolute_mde_ns,
        max_pairs=max_pairs,
    )
    required_pairs = max(relative_count.required_pairs, absolute_count.required_pairs)
    exceeds_cap = required_pairs > max_pairs
    identity_checks = {
        "identity_log_within_margin": _within_equivalence_margin(
            identity["log_ratio"], relative_mde
        ),
        "identity_absolute_within_margin": _within_equivalence_margin(
            identity["absolute_ns"], absolute_mde_ns
        ),
    }
    order_checks = {
        "order_log_within_margin": _within_equivalence_margin(
            order_effect["log_ratio"], relative_mde
        ),
        "order_absolute_within_margin": _within_equivalence_margin(
            order_effect["absolute_ns"], absolute_mde_ns
        ),
    }
    checks = {
        **identity_checks,
        **order_checks,
        "identity_within_equivalence_margin": all(identity_checks.values()),
        "order_effect_within_equivalence_margin": all(order_checks.values()),
        "required_pairs_fit_cap": not exceeds_cap,
    }
    stable = (
        checks["identity_within_equivalence_margin"]
        and checks["order_effect_within_equivalence_margin"]
        and checks["required_pairs_fit_cap"]
    )
    reasons = [
        name
        for name, passed in {**identity_checks, **order_checks}.items()
        if not passed
    ]
    if exceeds_cap:
        reasons.append("required_pairs_fit_cap")
    return {
        "status": "stable" if stable else "inconclusive",
        "stable": stable,
        "complete_pairs": len(pairs),
        "expected_pairs": expected_pairs,
        "identity_bounds": bounds_with_relative_percent(identity),
        "order_effect_bounds": bounds_with_relative_percent(order_effect),
        "sigma": {"log_ratio": sigma_log, "absolute_ns": sigma_absolute},
        "required_pairs": {
            "relative": asdict(relative_count),
            "absolute": asdict(absolute_count),
            "maximum": required_pairs,
            "exceeds_cap": exceeds_cap,
        },
        "checks": checks,
        "equivalence_margins": {
            "log_ratio": relative_mde,
            "absolute_ns": absolute_mde_ns,
        },
        "reasons": reasons,
        "bootstrap": {
            "seed": bootstrap_seed,
            "resamples": bootstrap_resamples,
            "confidence_level": confidence_level,
            "confidence_family_size": confidence_family_size,
            "multiplicity_adjustment": "bonferroni",
            "interval": "two_sided",
        },
    }


def low_latency_calibration(
    *,
    baseline_ns: float,
    side_summaries: dict[str, dict[str, Any]],
    max_pairs: int,
) -> dict[str, Any]:
    """Freeze row thresholds from independent A/A observations before fresh A/B."""
    checks: dict[str, bool] = {}
    noise = {"log_ratio": 0.0, "absolute_ns": 0.0}
    for side in ("base", "head"):
        summary = side_summaries[side]
        complete = (
            summary.get("status") != "contract_failure"
            and summary.get("complete_pairs") == summary.get("expected_pairs")
        )
        checks[f"{side}_complete_aa_schedule"] = complete
        if not complete:
            continue
        for effect in ("identity", "order_effect"):
            for metric in noise:
                bounds = summary[f"{effect}_bounds"][metric]
                checks[f"{side}_{effect}_{metric}_contains_zero"] = (
                    bounds["lower"] <= 0.0 <= bounds["upper"]
                )
                noise[metric] = max(
                    noise[metric], abs(bounds["lower"]), abs(bounds["upper"])
                )
    checks["relative_noise_within_limit"] = noise["log_ratio"] <= -math.log(0.95)
    checks["absolute_noise_within_limit"] = noise["absolute_ns"] <= min(
        1_000.0, 0.05 * baseline_ns
    )
    relative_threshold = max(-math.log(0.90), 3.0 * noise["log_ratio"])
    absolute_threshold = max(
        1_000.0,
        min(50_000.0, baseline_ns * -math.expm1(-relative_threshold)),
        3.0 * noise["absolute_ns"],
    )
    power: dict[str, dict[str, Any]] = {}
    for side, summary in side_summaries.items():
        if not checks[f"{side}_complete_aa_schedule"]:
            continue
        power[side] = {
            metric: asdict(required_pair_count(
                sigma=summary["sigma"][metric],
                minimum_detectable_effect=threshold,
                max_pairs=max_pairs,
            ))
            for metric, threshold in (
                ("log_ratio", relative_threshold), ("absolute_ns", absolute_threshold)
            )
        }
    required = max(
        (count["required_pairs"] for side in power.values() for count in side.values()),
        default=8,
    )
    checks["required_pairs_fit_cap"] = required <= max_pairs
    return {
        "mode": "low_latency_public_operation",
        "baseline_ns": baseline_ns,
        "noise": noise,
        "thresholds": {"log_ratio": relative_threshold, "absolute_ns": absolute_threshold},
        "required_pairs": required,
        "power": power,
        "checks": checks,
        "stable": all(checks.values()),
        "reasons": [name for name, passed in checks.items() if not passed],
    }
