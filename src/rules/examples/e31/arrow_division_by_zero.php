<?php

function arrow_division_by_zero(): Closure
{
    return fn(int $divisor): float => 1 / $divisor;
}
