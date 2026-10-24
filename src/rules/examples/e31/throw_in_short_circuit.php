<?php

function throw_in_short_circuit(?string $value): string
{
    $value = $value ?? throw new \RuntimeException('missing');

    return $value;
}

function throw_in_boolean_expression(bool $debug): void
{
    $debug && throw new \RuntimeException('debug');

    echo "reachable";
}

function throw_in_ternary(bool $debug): void
{
    $debug ? throw new \RuntimeException('debug') : null;

    echo "reachable";
}
