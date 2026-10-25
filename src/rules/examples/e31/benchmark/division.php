<?php

function benchmark_literal_zero(): int
{
    return 1 / 0;
}

function benchmark_known_zero(): int
{
    $divisor = 0;

    return 10 / $divisor;
}

function benchmark_nonzero(): int
{
    return 10 / 2;
}

function benchmark_guarded(int $divisor): float
{
    if ($divisor === 0) {
        return 0.0;
    }

    return 1 / $divisor;
}

function benchmark_modulo_zero(): int
{
    $divisor = 0;

    return 10 % $divisor;
}

function benchmark_intdiv_zero(): int
{
    return intdiv(10, 0);
}

function benchmark_intdiv_overflow(): int
{
    return intdiv(-9223372036854775808, -1);
}

function benchmark_intdiv_ok(): int
{
    return intdiv(10, 2);
}
