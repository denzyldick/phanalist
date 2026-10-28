<?php

function retain_summary_use(array $items): void
{
    foreach ($items as $item) {
        retain_helper($items);
    }
}
