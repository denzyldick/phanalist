<?php

function assertion_guard(int $divisor): float
{
    assert($divisor !== 0);

    return 1 / $divisor;
}
