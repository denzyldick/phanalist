<?php

function closure_use_division_by_zero(): Closure
{
    $zero = 0;

    return function () use ($zero): float {
        return 1 / $zero;
    };
}
