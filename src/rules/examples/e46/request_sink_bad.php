<?php

function request_sink_bad(): string
{
    return str_repeat('x', $_GET['size']);
}
