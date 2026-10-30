<?php

function retain_summary_unused(array $items): void
{
    foreach ($items as $item) {
        retain_helper($items);
    }
}
