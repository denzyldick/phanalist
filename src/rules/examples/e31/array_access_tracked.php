<?php

function tracked_array_zero(): float
{
    $config = ['divisor' => 0];

    return 10 / $config['divisor'];
}

function tracked_array_missing_default(): float
{
    $config = [];

    return 10 / ($config['divisor'] ?? 0);
}
