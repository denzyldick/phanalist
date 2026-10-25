<?php

function benchmark_bad_return(): int
{
    return 'not an int';
}

function benchmark_good_return(): int
{
    return 1;
}

/**
 * @return int
 */
function benchmark_bad_doc()
{
    return 'not an int';
}

/**
 * @return int
 */
function benchmark_good_doc()
{
    return 1;
}

function benchmark_null_call(): void
{
    $value = null;
    $value->run();
}

function benchmark_nullsafe_call(): void
{
    $value = null;
    $value?->run();
}
