<?php

function request_method_bad(object $request): void
{
    foreach ($request->input('items') as $item) {
        echo $item;
    }
}
