<?php

/**
 * @param iterable<array<string, int>> $rows
 */
function pgacc_average_score_unknown_strict_guard(iterable $rows): float
{
    $Scount = 0;
    foreach ($rows as $row) {
        $Scount += $row['score'];
    }
    if ($Scount !== 0) {
        return 100 / $Scount;
    }
    return 0.0;
}
