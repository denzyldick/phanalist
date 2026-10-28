<?php

function request_merge_bad(): void
{
    $items = array_merge($_GET['first'] ?? [], $_GET['second'] ?? []);
    foreach ($items as $item) {
        echo $item;
    }
}
