<?php

function multipartBoundary(): string
{
    return hash('xxh128', uniqid((string) time()));
}
