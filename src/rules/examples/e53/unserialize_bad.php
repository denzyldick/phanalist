<?php

function restore(string $payload): object
{
    $first = unserialize($payload);
    $second = unserialize($payload, []);
    $third = unserialize($payload, ['allowed_classes' => true]);

    return $first ?? $second ?? $third;
}
