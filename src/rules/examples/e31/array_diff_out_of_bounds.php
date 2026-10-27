<?php

/**
 * @param array<int, string> $plan
 * @param array<int, string> $tried
 */
function array_diff_tail(array $plan, array $tried): string
{
    $remaining = array_values(array_diff($plan, $tried));

    return $remaining[count($plan)];
}
