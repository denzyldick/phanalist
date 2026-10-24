<?php

/**
 * @phpstan-assert non-zero-int $value
 */
function assume_non_zero($value): void
{
}

function phpdoc_assert_function(int $divisor): float
{
    assume_non_zero($divisor);

    return 1 / $divisor;
}
