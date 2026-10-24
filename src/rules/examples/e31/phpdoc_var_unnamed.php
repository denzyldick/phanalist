<?php

function phpdoc_var_unnamed(): float
{
    /** @var non-zero-int */
    $divisor = unknown_value();

    return 1 / $divisor;
}
