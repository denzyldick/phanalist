<?php

/**
 * @param iterable<array<string, int>> $rows
 */
function pgacc_average_score_guarded(iterable $rows): float
{
    $Scount = 0;
    foreach ($rows as $row) {
        $Scount++;
    }

    if ($Scount > 0) {
        return 100 / $Scount;
    }

    return 0.0;
}
