<?php

function restore(string $payload, array $options): object
{
    $first = unserialize($payload, ['allowed_classes' => false]);
    $second = unserialize($payload, ['allowed_classes' => [TrustedValue::class]]);
    $third = unserialize($payload, $options);
    $fourth = unserialize(serialize($first));
    $fifth = unserialize('a:0:{}');

    return $first ?? $second ?? $third ?? $fourth ?? $fifth;
}
