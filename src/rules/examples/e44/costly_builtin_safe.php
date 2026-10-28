<?php

function matching(array $needles): array
{
    $found = [];
    $allowed = ['a', 'b', 'c'];

    foreach ($needles as $needle) {
        if (in_array($needle, $allowed, true)) {
            $found[] = $needle;
        }
    }

    return $found;
}
