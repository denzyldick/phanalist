<?php

function phpdoc_var_nonzero(): float
{
    /** @var non-zero-int $divisor */
    $divisor = unknown_value();

    return 1 / $divisor;
}
