<?php

function match_guarded(int $value): float
{
    return match ($value) {
        1 => 1 / $value,
        default => 0.0,
    };
}
