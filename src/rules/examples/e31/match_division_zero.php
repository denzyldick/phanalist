<?php

function match_unguarded(int $value): float
{
    return match ($value) {
        0 => 1 / $value,
        default => 0.0,
    };
}
