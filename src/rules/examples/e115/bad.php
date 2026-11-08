<?php
// Bad: XXE via libxml with user input
$xml = $_GET['xml'] ?? '';
domdocument::loadxml($xml);
