#!/usr/bin/env python3
"""Deterministic stdin/stdout adapter for optional RocketPy comparison runs.

This is intentionally a narrow mapping of Ascent's point-mass inputs. It does
not fetch weather, use the clock, or claim geometry/aerodynamics that the
Ascent `Rocket` input does not contain.
"""

from __future__ import annotations

import json
import math
import sys
import traceback
from importlib.metadata import version

REQUEST_SCHEMA = "ascent-rocketpy-request-v1"
RESPONSE_SCHEMA = "ascent-rocketpy-response-v1"
T0_K = 288.15
P0_PA = 101_325.0
LAPSE_K_PER_M = 0.0065
R_SPECIFIC = 287.053


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _custom_atmosphere(model: object) -> tuple[list[list[float]], list[list[float]]]:
    """Return deterministic [altitude, value] profiles for RocketPy.

    RocketPy receives only the generated custom profile. The standard branch
    reproduces Ascent's 1976-troposphere equation on a fixed 100 m grid; the
    constant-density branch holds 288.15 K and derives matching pressure.
    """

    altitudes = list(range(0, 11_001, 100))
    if model == "Standard":
        temperatures = [[float(h), T0_K - LAPSE_K_PER_M * h] for h in altitudes]
        pressures = [
            [float(h), P0_PA * ((T0_K - LAPSE_K_PER_M * h) / T0_K) ** (9.80665 / (R_SPECIFIC * LAPSE_K_PER_M))]
            for h in altitudes
        ]
        return pressures, temperatures
    if isinstance(model, dict) and isinstance(model.get("ConstantDensity"), (int, float)):
        density = float(model["ConstantDensity"])
        _require(density > 0.0, "constant atmosphere density must be positive")
        pressure = density * R_SPECIFIC * T0_K
        return [[float(h), pressure] for h in altitudes], [[float(h), T0_K] for h in altitudes]
    raise ValueError("unsupported Ascent atmosphere; expected Standard or ConstantDensity")


def _event(kind: str, time_s: float, altitude_m: float, velocity_ms: float) -> dict[str, float | str]:
    return {
        "kind": kind,
        "t_s": float(time_s),
        "altitude_m": float(altitude_m),
        "velocity_ms": float(velocity_ms),
    }


def run(request: dict) -> dict:
    _require(request.get("schema_version") == REQUEST_SCHEMA, "unsupported request schema")
    rocket_input = request["rocket"]
    motor_input = request["motor"]
    environment_input = request["environment"]
    config = request["config"]

    dry_mass_kg = float(rocket_input["dry_mass_kg"])
    total_mass_kg = float(motor_input["total_mass_kg"])
    propellant_mass_kg = float(motor_input["propellant_mass_kg"])
    _require(dry_mass_kg > 0.0, "rocket dry_mass_kg must be positive")
    _require(total_mass_kg > propellant_mass_kg > 0.0, "motor masses must be positive")
    drag = rocket_input.get("drag")
    _require(isinstance(drag, dict), "RocketPy bridge requires Ascent drag cd and reference area")
    cd = float(drag["cd"])
    reference_area_m2 = float(drag["reference_area_m2"])
    _require(cd >= 0.0 and reference_area_m2 > 0.0, "drag cd/area must be non-negative/positive")
    thrust_curve = [[0.0, 0.0]] + [[float(t), float(thrust)] for t, thrust in motor_input["thrust_curve"]]
    _require(len(thrust_curve) > 1 and thrust_curve[-1][0] > 0.0, "motor thrust curve is empty")
    burn_time_s = thrust_curve[-1][0]
    max_time_s = float(config["max_time_s"])
    timestep_s = float(config["dt_s"])
    _require(max_time_s > 0.0 and timestep_s > 0.0, "simulation times must be positive")

    # Imported lazily so a missing RocketPy package is a clean bridge failure.
    from rocketpy import Environment, Flight, GenericMotor, Rocket

    pressure, temperature = _custom_atmosphere(environment_input["atmosphere"])
    environment = Environment(
        gravity=float(environment_input["gravity_ms2"]),
        date=(2020, 1, 1, 0),
        latitude=0.0,
        longitude=0.0,
        elevation=0.0,
        timezone="UTC",
        max_expected_height=11_000.0,
    )
    environment.set_atmospheric_model(
        "custom_atmosphere",
        pressure=pressure,
        temperature=temperature,
        wind_u=0.0,
        wind_v=0.0,
    )

    # Ascent has a sampled thrust curve and masses, but no grain geometry.
    # GenericMotor faithfully carries those supplied quantities without
    # inventing a grain configuration.
    motor = GenericMotor(
        thrust_source=thrust_curve,
        burn_time=burn_time_s,
        chamber_radius=0.01,
        chamber_height=0.07,
        chamber_position=0.035,
        propellant_initial_mass=propellant_mass_kg,
        nozzle_radius=0.003,
        dry_mass=total_mass_kg - propellant_mass_kg,
        dry_inertia=(0.0, 0.0, 0.0),
    )
    radius_m = math.sqrt(reference_area_m2 / math.pi)
    rocket = Rocket(
        radius=radius_m,
        mass=dry_mass_kg,
        # Ascent does not carry axial geometry/inertia; tiny diagonal inertia
        # is a numerical placeholder for RocketPy's 3-DOF point-mass run.
        inertia=(1e-6, 1e-6, 1e-6),
        power_off_drag=cd,
        power_on_drag=cd,
        center_of_mass_without_motor=0.0,
    )
    rocket.add_motor(motor, position=0.0)
    recovery = rocket_input.get("recovery")
    if recovery is not None:
        chute_cd_s = float(recovery["chute_cd"]) * float(recovery["chute_area_m2"])
        _require(chute_cd_s > 0.0, "recovery chute CdA must be positive")
        rocket.add_parachute(
            name="ascent-apogee",
            cd_s=chute_cd_s,
            trigger="apogee",
            sampling_rate=100,
            lag=0.0,
            noise=(0.0, 0.0, 0.0),
        )

    flight = Flight(
        rocket=rocket,
        environment=environment,
        rail_length=float(environment_input["rail_length_m"]),
        inclination=90.0,
        heading=0.0,
        max_time=max_time_s,
        max_time_step=timestep_s,
        time_overshoot=False,
        verbose=False,
        simulation_mode="3 DOF",
    )
    burnout_altitude = flight.z.get_value_opt(burn_time_s)
    burnout_velocity = flight.vz.get_value_opt(burn_time_s)
    rail_time = flight.out_of_rail_time
    events = [
        _event("Liftoff", 0.0, 0.0, 0.0),
        _event("RailExit", rail_time, flight.z.get_value_opt(rail_time), flight.out_of_rail_velocity),
        _event("Burnout", burn_time_s, burnout_altitude, burnout_velocity),
        _event("Apogee", flight.apogee_time, flight.apogee, 0.0),
    ]
    if recovery is not None:
        events.append(_event("RecoveryDeploy", flight.apogee_time, flight.apogee, 0.0))
    events.append(_event("Landing", flight.t_final, 0.0, flight.impact_velocity))
    return {
        "schema_version": RESPONSE_SCHEMA,
        "rocketpy_version": version("rocketpy"),
        "summary": {
            "apogee_m": flight.apogee,
            "apogee_time_s": flight.apogee_time,
            "burnout_time_s": burn_time_s,
            "burnout_velocity_ms": burnout_velocity,
            "max_velocity_ms": flight.max_speed,
            "rail_exit_velocity_ms": flight.out_of_rail_velocity,
            "landing_time_s": flight.t_final,
            "landing_velocity_ms": flight.impact_velocity,
            "events": events,
            "timestep_s": timestep_s,
            "steps": len(flight.solution),
        },
    }


def main() -> int:
    try:
        request = json.load(sys.stdin)
        result = run(request)
        print(json.dumps(result, allow_nan=False, separators=(",", ":")))
        return 0
    except Exception as error:  # diagnostics must never contaminate stdout
        print(f"RocketPy bridge error: {error}", file=sys.stderr)
        traceback.print_exc(file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
