<?php

/**
 * @param array<int, string> $plan
 * @param array<int, string> $tried
 */
function array_diff_first(array $plan, array $tried): ?string
{
    $remaining = array_values(array_diff($plan, $tried));

    if ($remaining === []) {
        return null;
    }

    return $remaining[0];
}
