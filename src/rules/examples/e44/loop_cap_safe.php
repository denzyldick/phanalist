<?php

function loop_cap_safe(array $items): void
{
    for ($i = 0; $i < count($items) && $i < 10; $i++) {
        foreach ($items as $item) {
            echo $item;
        }
    }
}
