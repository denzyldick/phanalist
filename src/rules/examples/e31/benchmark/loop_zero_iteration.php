<?php

/**
 * @param iterable<array<string, int>> $rows
 */
function pgacc_average_score(iterable $rows): float
{
    $Scount = 0;
    foreach ($rows as $row) {
        $Scount++;
    }

    return 100 / $Scount;
}
