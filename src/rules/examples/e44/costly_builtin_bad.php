<?php

function matching(array $items, array $needles): array
{
    $found = [];

    foreach ($needles as $needle) {
        if (in_array($needle, $items, true)) {
            $found[] = $needle;
        }
    }

    return $found;
}
