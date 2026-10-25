<?php

function pgacc_average_unknown_score(iterable $rows): float
{
    $Scount = 0;
    foreach ($rows as $row) {
        $Scount += $row['score'];
    }

    return 100 / $Scount;
}
