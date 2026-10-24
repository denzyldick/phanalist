<?php

/**
 * @phpstan-assert non-zero-int $value
 */
function documented_but_unrelated($value): void
{
}

function phpdoc_assert_unrelated(int $divisor): float
{
    return 1 / $divisor;
}
