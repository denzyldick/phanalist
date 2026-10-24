<?php

function phpdoc_var_named_existing(int $divisor): float
{
    /** @var positive-int $divisor */

    return 1 / $divisor;
}
