<?php

function method_use(ResourceMethodHelper $helper, array $items): void
{
    foreach ($items as $item) {
        $helper->nested($items);
    }
}
