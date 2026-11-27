import math
from dataclasses import dataclass

POLE_LAT = 90.0
HALF_TURN_DEG = 180.0


@dataclass(frozen=True)
class GeographicCoordinate:
    lat: float
    lon: float

    def __post_init__(self) -> None:
        if (
            not math.isfinite(self.lat)
            or not math.isfinite(self.lon)
            or not -POLE_LAT <= self.lat <= POLE_LAT
            or not -HALF_TURN_DEG <= self.lon <= HALF_TURN_DEG
        ):
            message = "coordinate must have finite latitude and longitude in geographic range"
            raise ValueError(message)


@dataclass(frozen=True)
class SphericalCircle:
    centre: GeographicCoordinate
    radius_km: float
    earth_radius_km: float

    def __post_init__(self) -> None:
        if not math.isfinite(self.earth_radius_km) or self.earth_radius_km <= 0.0:
            message = "earth radius must be finite and positive"
            raise ValueError(message)
        if (
            not math.isfinite(self.radius_km)
            or self.radius_km <= 0.0
            or self.radius_km >= math.pi * self.earth_radius_km
        ):
            message = (
                f"circle radius {self.radius_km} km must be finite and between zero and "
                f"half the circumference on a {self.earth_radius_km} km sphere"
            )
            raise ValueError(message)
