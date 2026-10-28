<?php
function for_bad(array $items): void
{
    for ($i = 0; $i < count($items); $i++) {
        for ($j = 0; $j < count($items); $j++) {
            echo $i + $j;
        }
    }
}
