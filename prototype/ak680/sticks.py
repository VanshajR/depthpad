"""Opposing keys -> one stick axis.

A port of HallJoy's ResolveAxis (src/HallJoyProject/HallJoy/configured_xusb_builder.cpp,
AGPL-3.0, PashOK7/HallJoy) so both tools resolve key pairs the same way:

  * default: plus - minus;
  * last_key_priority (SOCD "last input wins"): while both keys are held the
    most recently pressed one wins; a held key pushed deeper again by
    `sensitivity` retakes priority;
  * snappy: the axis takes the deeper key's full value in the winning direction
    instead of the difference.

Input values are already curved (0..1); a key counts as pressed at 0.10.
"""
PRESSED = 0.10
EQUAL_EPSILON = 0.002


class AxisResolver:
    def __init__(self, snappy=False, last_key_priority=False, sensitivity=0.02):
        self.snappy = snappy
        self.last_key_priority = last_key_priority
        self.sensitivity = min(0.95, max(0.02, sensitivity))
        self.prev_minus = self.prev_plus = False
        self.last_direction = 0
        self.minus_valley = self.plus_valley = 1.0

    def resolve(self, minus, plus):
        if not self.snappy and not self.last_key_priority:
            return plus - minus

        minus_down, plus_down = minus >= PRESSED, plus >= PRESSED
        if minus_down and not self.prev_minus:
            self.last_direction = -1
        if plus_down and not self.prev_plus:
            self.last_direction = 1

        if self.last_key_priority:
            if not minus_down:
                self.minus_valley = 1.0
            elif not self.prev_minus:
                self.minus_valley = minus
            else:
                self.minus_valley = min(self.minus_valley, minus)
                if minus - self.minus_valley >= self.sensitivity:
                    self.last_direction = -1
                    self.minus_valley = minus
            if not plus_down:
                self.plus_valley = 1.0
            elif not self.prev_plus:
                self.plus_valley = plus
            else:
                self.plus_valley = min(self.plus_valley, plus)
                if plus - self.plus_valley >= self.sensitivity:
                    self.last_direction = 1
                    self.plus_valley = plus

        self.prev_minus, self.prev_plus = minus_down, plus_down

        maximum = max(minus, plus)
        if maximum <= 0.0001:
            return 0.0
        if self.last_key_priority:
            if minus_down and not plus_down:
                return -minus
            if plus_down and not minus_down:
                return plus
            if minus_down and plus_down:
                direction = self.last_direction or (1 if plus >= minus else -1)
                magnitude = maximum if self.snappy else (plus if direction > 0 else minus)
                return magnitude if direction > 0 else -magnitude

        if self.snappy:
            difference = plus - minus
            if abs(difference) > EQUAL_EPSILON:
                return maximum if difference > 0 else -maximum
            if self.last_direction > 0:
                return maximum
            if self.last_direction < 0:
                return -maximum
            return 0.0
        return plus - minus
