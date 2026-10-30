<?php

function restore(string $payload, array $options): object
{
    $first = unserialize($payload, ['allowed_classes' => false]);
    $second = unserialize($payload, ['allowed_classes' => [TrustedValue::class]]);
    $third = unserialize($payload, $options);
    $fourth = unserialize(serialize($first));
    $fifth = unserialize('a:0:{}');
    $sixth = unserialize(data: $payload, options: ['allowed_classes' => false]);
    $seventh = unserialize($payload, options: ['allowed_classes' => [TrustedValue::class]]);

    return $first ?? $second ?? $third ?? $fourth ?? $fifth ?? $sixth ?? $seventh;
}
