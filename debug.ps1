function run { param($name, $line, $expression)
	$prog = "windbg"
	$as = '-c',"`$`$>a< trydebug.txt $($name) $($line) $($expression);g","tmp\$($name).exe"
	& $prog $as
}

function runmenu { param($line, $expression)
	$name = "RunProcess"
	$prog = "windbg"
	# $as = '-c',".open $($name).c",'-c',"bp ``$($name).c:$($line)``;g","tmp\$($name).exe", '--namedCommand', 'ld:dir', '--loadCommand', 'ld'

	$as = '-c',"`$`$>a< trydebug.txt $($name) $($line) $($expression);g","tmp\$($name).exe","--namedCommand","ld:dir","--loadCommand","ld"
	& $prog $as
}

function runmenuex { param($line, $expression)
	$name = "RunProcess"
	$prog = "windbg"
	$as = '-c',".open $($name).c",'-c',"bp ``$($name).c:$($line)`` `"`"`"$($expression);gc`"`"`";g","tmp\$($name).exe", '--namedCommand', 'ld:dir', '--loadCommand', 'ld'
	& $prog $as
}

# windbg -c "$$>a< trydebug.txt RunProcess 1497 argv[i];g" tmp\RunProcess.exe --namedCommand ld:dir --loadCommand ld
