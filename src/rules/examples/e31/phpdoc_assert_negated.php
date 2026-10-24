<?php

/**
 * @phpstan-assert !null $value
 */
function assume_not_null($value): void
{
}

function phpdoc_assert_negated(): int
{
    assume_not_null(null);

    return 1;
}
