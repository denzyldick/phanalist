<?php

function benchmark_after_return(): int
{
    return 1;

    echo 'unreachable';
}

function benchmark_branch_return(bool $flag): int
{
    if ($flag) {
        return 1;
    }

    return 2;
}

function benchmark_switch_fallthrough(): float
{
    $divisor = 0;

    switch (0) {
        case 0:
            $divisor = 0;
        case 1:
            return 1 / $divisor;
    }

    return 0.0;
}

function benchmark_switch_guarded(int $value): float
{
    switch ($value) {
        case 0:
            break;
        case 1:
            return 1 / $value;
        default:
            return 0.0;
    }

    return 0.0;
}

function benchmark_assert_false(): void
{
    assert(false);
}

function benchmark_assert_guard(int $divisor): float
{
    assert($divisor !== 0);

    return 1 / $divisor;
}
