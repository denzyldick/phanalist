<?php

function closure_division_by_zero(): Closure
{
    return function (int $divisor): float {
        return 1 / $divisor;
    };
}
