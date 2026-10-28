<?php

function decodedName(string $json): ?string
{
    $data = json_decode($json, true);
    if (!is_array($data)) {
        return null;
    }

    return $data['name'] ?? null;
}

function firstItem(string $json): mixed
{
    return json_decode($json, true)[0] ?? null;
}

function hasName(string $json): bool
{
    return isset(json_decode($json, true)['name']);
}

function throwingName(string $json): string
{
    return json_decode($json, true, 512, JSON_THROW_ON_ERROR)['name'];
}

function namedThrowingName(string $json): string
{
    return json_decode($json, flags: JSON_THROW_ON_ERROR)->name;
}
